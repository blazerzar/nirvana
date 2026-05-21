<script setup lang="ts">
import { useAllTasksStore } from "../stores/allTasks";
import CreateTaskModal from "./modals/CreateTaskModal.vue";
import EditTaskModal from "./modals/EditTaskModal.vue";
import PublishTaskModal from "./modals/PublishTaskModal.vue";
import StartTaskModal from "./modals/StartTaskModal.vue";

const tasks = useAllTasksStore();
</script>

<template>
    <Transition name="modal">
        <div
            v-if="tasks.activeModal"
            class="fixed inset-0 z-20 grid place-items-center bg-[rgba(7,8,9,0.68)] p-[18px] backdrop-blur-[5px]"
            @click.self="tasks.closeModal()"
        >
            <StartTaskModal v-if="tasks.activeModal === 'start'" />
            <CreateTaskModal v-else-if="tasks.activeModal === 'create'" />
            <EditTaskModal v-else-if="tasks.activeModal === 'edit'" />
            <PublishTaskModal v-else />
        </div>
    </Transition>
</template>
